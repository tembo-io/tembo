from dataclasses import dataclass, field
import json
import logging
from typing import Any, Optional

from llama_index.core import SimpleDirectoryReader
from llama_index.core.node_parser import SentenceSplitter
import psycopg


@dataclass
class TemboRAG:
    project_name: str
    chunk_size: Optional[int] = None
    chat_model: str = "gpt-3.5-turbo"
    sentence_transformer: str = "sentence-transformers/all-MiniLM-L12-v2"
    connection_string: Optional[str] = None
    table_name: str = "vectorize._data_{project_name}"

    # post-init
    sentence_splitter: SentenceSplitter = field(
        default_factory=SentenceSplitter, init=False
    )

    def __post_init__(self):
        chunk_size = self.chunk_size or get_context_size(self.chat_model)
        self.sentence_splitter = SentenceSplitter(chunk_size=chunk_size)
        self.chunk_size = chunk_size

    def query(
        self,
        query: str,
        connection_string: Optional[str] = None,
        chat_model: Optional[str] = None,
        prompt_template: Optional[str] = None,
        num_context: Optional[int] = None,
        force_trim: Optional[bool] = None,
        api_key: Optional[str] = None,
    ):
        connection_string = connection_string or self.connection_string
        chat_model = chat_model or self.chat_model

        if not connection_string:
            raise ValueError("No connection string provided")
        q, bind_params = self._prepare_query_params(
            query=query,
            chat_model=chat_model,
            prompt_template=prompt_template,
            num_context=num_context,
            force_trim=force_trim,
            api_key=api_key,
        )

        with psycopg.connect(connection_string, autocommit=True) as conn:
            cur = conn.cursor()
            resp = cur.execute(q, bind_params).fetchone()

        return ChatResponse(**resp[0])  # type: ignore

    def _prepare_query_params(
        self,
        query: str,
        chat_model: Optional[str] = None,
        prompt_template: Optional[str] = None,
        num_context: Optional[int] = None,
        force_trim: Optional[bool] = None,
        api_key: Optional[str] = None,
    ) -> tuple[str, tuple]:
        q = "SELECT vectorize.rag(agent_name => %s,query => %s,chat_model => %s"
        bind_params = (self.project_name, query, chat_model)
        if prompt_template is not None:
            q = q + ",task => %s"
            bind_params = bind_params + (prompt_template,)  # type: ignore
        if api_key is not None:
            q = q + ",api_key => %s"
            bind_params = bind_params + (api_key,)  # type: ignore
        if num_context is not None:
            q = q + ",num_context => %s"
            bind_params = bind_params + (num_context,)  # type: ignore
        if force_trim is not None:
            q = q + ",force_trim => %s"
            bind_params = bind_params + (force_trim,)  # type: ignore
        q = q + ");"
        return q, bind_params

    def prepare_from_directory(
        self, document_dir: str, **kwargs
    ) -> list[tuple[str, str, str, str]]:
        documents = SimpleDirectoryReader(document_dir).load_data()
        chunks = self.sentence_splitter.get_nodes_from_documents(documents, **kwargs)
        chunks_for_copy: list[tuple[str, str, str, str]] = []
        for chunk in chunks:
            chunks_for_copy.append(
                (
                    chunk.metadata["file_name"],
                    chunk.id_,
                    json.dumps(chunk.metadata),
                    chunk.get_content().replace("'", "''"),
                )
            )
        logging.info("Prepared %s chunks", len(chunks_for_copy))
        return chunks_for_copy

    def load_documents(
        self,
        documents: list[tuple[str, str, str, str]],
        connection_string: Optional[str] = None,
    ):
        connection_string = connection_string or self.connection_string
        if not connection_string:
            raise ValueError("No connection string provided")
        self._init_table(self.project_name, connection_string)
        table = self.table_name.format(project_name=self.project_name)
        self._load_docs(table, documents, connection_string)

    def init_rag(
        self, connection_string: Optional[str] = None, transformer: Optional[str] = None
    ):
        connection_string = connection_string or self.connection_string
        if not connection_string:
            raise ValueError("No connection string provided")

        xformer = transformer or self.sentence_transformer
        q = """
        SELECT vectorize.init_rag(
            agent_name => %s,
            table_name => %s,
            schema => %s,
            unique_record_id => 'record_id',
            "column" => 'content',
            transformer => %s
        );
        """
        schema, table = self.table_name.format(project_name=self.project_name).split(
            "."
        )
        with psycopg.connect(connection_string, autocommit=True) as conn:
            cur = conn.cursor()
            cur.execute(q, (self.project_name, table, schema, xformer))

    def _load_docs(
        self,
        table: str,
        documents: list[tuple[str, str, str, str]],
        connection_string: str,
    ):
        with psycopg.connect(connection_string, autocommit=True) as conn:
            cur = conn.cursor()
            sql = f"COPY {table} (document_name, chunk_id, meta, content) FROM STDIN"
            # log every 10% completion
            num_chunks = len(documents)
            deca = num_chunks // 10
            with cur.copy(sql) as copy:
                for i, row in enumerate(documents):
                    if i % deca == 0:
                        logging.info("writing row %s / %s", i, num_chunks)
                    copy.write_row(row)

    def _init_table(self, project_name: str, connection_string: str):
        table = self.table_name.format(project_name=project_name)
        q = f"""
        CREATE TABLE IF NOT EXISTS {table} (
            record_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            document_name TEXT NOT NULL,
            chunk_id TEXT NOT NULL,
            meta JSONB,
            content TEXT NOT NULL
        )
        """
        with psycopg.connect(connection_string, autocommit=True) as conn:
            cur = conn.cursor()
            cur.execute(q)

    def add_prompt_template(
        self,
        prompt_name: str,
        sys_prompt: str,
        user_prompt: str,
        connection_string: Optional[str] = None,
    ):
        """
        Adds a new prompt template to the vectorize.prompts table.

        :param prompt_name: The type or name of the prompt template.
        :param sys_prompt: The system's prompt text.
        :param user_prompt: The user-facing prompt text.
        :param connection_string: Optional; database connection string. If not provided, uses the instance's connection string.
        """
        connection_string = connection_string or self.connection_string
        if not connection_string:
            raise ValueError("No connection string provided")

        insert_query = """INSERT INTO vectorize.prompts (prompt_type, sys_prompt, user_prompt) VALUES (%s, %s, %s)"""

        # Execute the insert query with the provided parameters
        with psycopg.connect(connection_string) as conn:
            with conn.cursor() as cur:
                cur.execute(insert_query, (prompt_name, sys_prompt, user_prompt))


@dataclass
class ChatResponse:
    context: list[dict[str, Any]]
    chat_response: str


def get_context_size(model):
    if model.startswith("gpt-4-1106"):
        return 128000
    if model.startswith("gpt-4-32k"):
        return 32768
    if model.startswith("gpt-4"):
        return 8192
    if model.startswith("gpt-3.5-turbo-16k"):
        return 16384
    if model.startswith("gpt-3.5-turbo"):
        return 4096
    if model in ("text-davinci-002", "text-davinci-003"):
        return 4097
    if model in ("ada", "babbage", "curie"):
        return 2049
    if model == "code-cushman-001":
        return 2048
    if model == "code-davinci-002":
        return 8001
    if model == "davinci":
        return 2049
    if model in ("text-ada-001", "text-babbage-001", "text-curie-001"):
        return 2049
    if model == "text-embedding-ada-002":
        return 8192
    return 4096

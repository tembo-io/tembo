from dataclasses import dataclass, field
import json
import logging
from typing import Optional

from llama_index.core import SimpleDirectoryReader
from llama_index.core.node_parser import SentenceSplitter
import psycopg


@dataclass
class TemboRAGcontroller:
    project_name: str
    chunk_size: Optional[int] = None
    chat_model: str = "gpt-3.5-turbo"
    sentence_transformer: str = "sentence-transformers/all-MiniLM-L12-v2"
    connection_string: Optional[str] = None
    _table_name: str = "vectorize._data_{project_name}"

    # post-init
    sentence_splitter: SentenceSplitter = field(
        default_factory=SentenceSplitter, init=False
    )

    def __post_init__(self):
        chunk_size = self.chunk_size or get_context_size(self.chat_model)
        self.sentence_splitter = SentenceSplitter(chunk_size=chunk_size)
        self.chunk_size = chunk_size

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
                    chunk.get_content(),
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
        table = self._table_name.format(project_name=self.project_name)
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
        schema, table = self._table_name.format(project_name=self.project_name).split(
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
        table = self._table_name.format(project_name=project_name)
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

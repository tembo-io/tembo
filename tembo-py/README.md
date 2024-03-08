# tembo-py

The official Python client for Tembo.io

For more technical information and ways to get involved, please refer to the [contributing guide](./CONTRIBUTING.md).

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Prepare and Insert Sample Documents](#prepare-and-insert-sample-documents)
- [Adding Custom Prompts](#adding-custom-prompts)

## Prerequisites

- [Python](https://www.python.org/) - Programming language and companion `pip` package manager
- [Docker Engine](https://docs.docker.com/engine/install/) - For running local containers

## Installation

The [tembo-py library](https://pypi.org/project/tembo-py/) is hosted on pypi.org and can be installed using the following `pip` command.

```bash
pip install tembo-py
```

## Prepare Contextual Basis

Before jumping in, it's important to have material to offer the model as context.
The [RAG Stack official documentation](https://tembo.io/docs/tembo-stacks/rag#build-a-support-agent-with-tembo-rag) does a good job reviewing this in detail, so we will highlight some key points.

```bash
from tembo_py.rag import TemboRAG

rag = TemboRAG(
    project_name="tembo_support",
    chat_model="gpt-3.5-turbo",
    connection_string="postgresql://postgres:<your-password>@<your-TemboHost>:5432/postgres"
)

chunks = rag.prepare_from_directory("./tembo_docs")

rag.load_documents(chunks)
```

Now that the table is loaded into Postgres, you can run the following:

```python
rag.init_rag(connection_string="postgresql://postgres:wJj00hgPWnh5qalf@org-evan-test-inst-evan-mar-rag-test.data-1.use1.tembo-development.com:5432/postgres",
             transformer="sentence-transformers/all-MiniLM-L12-v2"
)
```

## Adding Custom Prompts

Within the [rag.py](./tembo_py/rag.py) file's `TemboRAG` class, the `add_prompt_template` method introduces the ability to add custom prompts.
For the purposes of this example we will consider a local environment.

#### 1. Running PostgreSQL

Before even touching Python, let's go ahead and start a Postgres instance.
If you'd like, you can run the following to clone and enter the `tembo-py` repository:

```bash
git clone https://github.com/tembo-io/tembo.git
```
```bash
cd tembo/tembo-py
```

Then run the following command to start a Postgres instance:

```bash
make run.postgres
```

From there, `psql` into Postgres and enable the [pg_vectorize](https://github.com/tembo-io/pg_vectorize) extension.

:bulb: Note that the password is by default `postgres`.

```bash
psql -h localhost -p 5432 -U postgres -W
```

```sql
CREATE EXTENSION vectorize CASCADE;
```

#### 2. Create a New Python File

Exit Postgres and create a new Python file:

```bash
touch example_tembo.py
```

#### 3. Define Your Custom Prompt

Using your preferred text editor or IDE, you can create the following script:

```python
     rag.add_prompt_template(
              <prompt_type>, # The title of the prompt.
              <sys_prompt>,  # Priming the system characteristics.
              <user_prompt>  # Any information brought by the user.
          )
```

This will look something like this:

```python
     rag.add_prompt_template(
              "booyah", 
              "You are a Postgres expert and are tasked with helping users find answers in Tembo documentation. You should prioritize answering questions using the provided context, but can draw from your expert Postgres experience where documentation is lacking. Avoid statements like based on the documentation... and also you love to say booyah! alot.",
              "Context information is below.\n---------------------\n{{ context_str }}\n---------------------\nGiven the Tembo documentation information and your expert Postgres knowledge, answer the question.\n Question: {{ query_str }}\nAnswer:"
          )
```

#### 4. Executing the Python File and Confirming Success

```python
if __name__ == "__main__":
    question = "Tell me a joke about the geospatial stack."
    prompt_template_name = "booyah" 
    print(f"Querying: {question}")
    result = ensure_prompt_and_query(question, prompt_template_name)
    print("Response:", result)
```


```bash
python3 example_tembo.py
```

If successful, you should see the following:

```text
Prompt 'example_prompt' successfully added to the database.
System Prompt: System prompt text for example
User Prompt: User prompt text for example
```

You can now enter Postgres to confirm the insertion of your custom prompt.

:bulb: Note that the password is by default `postgres`.

```bash
psql -h localhost -p 5432 -U postgres -W
```

As `pg_vectorize` was enabled above, run the following SELECT statement to confirm your new addition:

```sql
SELECT * FROM vectorize.prompts;
```

After running `\x` for better formatting, you will a new addition similar to the following:

```sql
-[ RECORD 2 ]-------------------------------
prompt_type | example_prompt
sys_prompt  | System prompt text for example
user_prompt | User prompt text for example
```

# tembo-py

The official Python client for Tembo.io

## Table of Contents

- [Installation](#installation)
- [Interacting with RAG](#interacting-with-rag)
- [Adding Custom Prompts](#adding-custom-prompts)

## Installation

The [tembo-py library](https://pypi.org/project/tembo-py/) is hosted on pypi.org and can be installed using the following `pip` command.

```bash
pip install tembo-py
```

## Interacting with RAG

Interacting with the RAG Stack requires processing documents in chunks and loading them in to Postgres.
The `tembo-py` client is designed for this type of work, which is outlined in detail within the [RAG Stack official documentation](https://tembo.io/docs/tembo-stacks/rag#build-a-support-agent-with-tembo-rag).

```python
from tembo_py.rag import TemboRAG

rag = TemboRAG(
    project_name="tembo_support",
    chat_model="gpt-3.5-turbo",
    connection_string="postgresql://postgres:<your-password>@<your-TemboHost>:5432/postgres"
)

chunks = rag.prepare_from_directory("./tembo_docs") # File path to your loadable data

rag.load_documents(chunks)
```

Now that the table is loaded into Postgres, you can run the following:

```python
rag.init_rag(
    transformer="sentence-transformers/all-MiniLM-L12-v2"
)
```

## Adding Custom Prompts

If you'd like to add a custom prompt, begin by confirming that [pg_vectorize](https://github.com/tembo-io/pg_vectorize) is enabled and that you've set your openai api key.

### 1. Connect to Postgres

```bash
psql postgresql://postgres:<your-password>@<your-TemboHost>:5432/postgres
```

From there, enable the `pg_vectorize` extension.

```sql
CREATE EXTENSION vectorize CASCADE;
```

The chat completion model only supports OpenAI (embeddings can come from more sources), for now.
Enter the OpenAI API key into the configuration below:

```sql
ALTER SYSTEM SET vectorize.openai_key TO '<your api key>';
```

```sql
SELECT pg_reload_conf();
```

### 2. Define Your Custom Prompt

The following outlines the parameters that you can adjust in your particular use case:

```python
rag.add_prompt_template(
    prompt_type="booyah", 
    sys_prompt="You are a Postgres expert and are tasked with helping users find answers in Tembo documentation. You should prioritize answering questions using the provided context, but can draw from your expert Postgres experience where documentation is lacking. Avoid statements like based on the documentation... and also you love to say booyah! alot.",
    user_prompt="Context information is below.\n---------------------\n{{ context_str }}\n---------------------\nGiven the Tembo documentation information and your expert Postgres knowledge, answer the question.\n Question: {{ query_str }}\nAnswer:"
)

query_string = "What are some real world applications of the geospatial stack?"
prompt_template_name = "booyah"

response = rag.query(query=query_string, prompt_template=prompt_template_name).chat_response

print(response)

Querying: What are some real world applications of the geospatial stack?
Response: Booyah! The Tembo Geospatial Stack opens up a world of possibilities for real-world applications leveraging its spatial database capabilities in Postgres. Some common applications include:
1. Mapping and spatial analysis for urban planning and development.
2. Location-based services for businesses such as geotargeted advertising or route optimization for delivery services.
3. Environmental monitoring and management, such as tracking wildlife habitats or analyzing climate data.
4. Disaster response and emergency management for planning evacuation routes or assessing impact areas.
5. Infrastructure design and management, like optimizing transportation networks or locating new facilities based on geographical factors.
The Tembo Geospatial Stack empowers users to efficiently handle spatial objects, execute location queries, and tackle GIS workloads for a wide range of industries and use cases.
```


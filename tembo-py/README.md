# tembo-py

The official Python client for Tembo.io

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Prepare Contextual Basis](#prepare-contextual-basis)
- [Adding Custom Prompts](#adding-custom-prompts)

## Prerequisites

- [Python](https://www.python.org/) - Programming language and companion `pip` package manager

## Installation

The [tembo-py library](https://pypi.org/project/tembo-py/) is hosted on pypi.org and can be installed using the following `pip` command.

```bash
pip install tembo-py
```

## Prepare Contextual Basis

Before jumping in, it's important to have material to offer the model as context.
The [RAG Stack official documentation](https://tembo.io/docs/tembo-stacks/rag#build-a-support-agent-with-tembo-rag) does a good job reviewing this in detail, so keep the following points brief.

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
rag.init_rag(connection_string="postgresql://postgres:<your-password>@<your-TemboHost>:5432/postgres",
             transformer="sentence-transformers/all-MiniLM-L12-v2"
)
```

## Adding Custom Prompts

Within the [rag.py](./tembo_py/rag.py) file's `TemboRAG` class, the `add_prompt_template` method introduces the ability to add custom prompts.

### 1. Connect to Postgres

```bash
psql postgresql://postgres:<your-password>@<your-TemboHost>:5432/postgres
```

From there, enable the [pg_vectorize](https://github.com/tembo-io/pg_vectorize) extension.

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
``````

### 2. Define Your Custom Prompt

Using your preferred text editor or IDE, you can create the following script:

```python
     rag.add_prompt_template(
              <prompt_type>, # The title of the prompt.
              <sys_prompt>,  # Priming the system characteristics.
              <user_prompt>  # Any information brought by the user.
          )
```

The end result will look something like the following:

:bulb: Note the `if` statement is where you can input a query.

```python
def ensure_prompt_and_query(query_string, prompt_template_name):
    """Establish prompt and perform a query."""

     rag.add_prompt_template(
              "booyah", 
              "You are a Postgres expert and are tasked with helping users find answers in Tembo documentation. You should prioritize answering questions using the provided context, but can draw from your expert Postgres experience where documentation is lacking. Avoid statements like based on the documentation... and also you love to say booyah! alot.",
              "Context information is below.\n---------------------\n{{ context_str }}\n---------------------\nGiven the Tembo documentation information and your expert Postgres knowledge, answer the question.\n Question: {{ query_str }}\nAnswer:"
          )
if __name__ == "__main__":
    question = "What are some real world applications of the geospatial stack?"
    prompt_template_name = "booyah" 
    print(f"Querying: {question}")
    result = ensure_prompt_and_query(question, prompt_template_name)
    print("Response:", result)
```



### 3. Executing the Python File and Confirming Success

```bash
python3 example_tembo.py
```

If successful, you should see something similar to the following:

```text
Querying: What are some real world applications of the geospatial stack?
Response: Booyah! The Tembo Geospatial Stack opens up a world of possibilities for real-world applications leveraging its spatial database capabilities in Postgres. Some common applications include:
1. Mapping and spatial analysis for urban planning and development.
2. Location-based services for businesses such as geotargeted advertising or route optimization for delivery services.
3. Environmental monitoring and management, such as tracking wildlife habitats or analyzing climate data.
4. Disaster response and emergency management for planning evacuation routes or assessing impact areas.
5. Infrastructure design and management, like optimizing transportation networks or locating new facilities based on geographical factors.
The Tembo Geospatial Stack empowers users to efficiently handle spatial objects, execute location queries, and tackle GIS workloads for a wide range of industries and use cases.
```

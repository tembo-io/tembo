# tembo-py

The official Python client for Tembo.io

For more technical information and ways to get involved, please refer to the [contributing guide](./CONTRIBUTING.md).

## Installation

The [tembo-py library](https://pypi.org/project/tembo-py/) is hosted on pypi.org and can be installed using the following `pip` command.

```bash
pip install tembo-py
```

## Example

### Adding Custom Prompts

Within the [rag.py](./tembo_py/rag.py) file's `TemboRAG` class, the `add_prompt_template` method introduces the ability to add custom prompts.
For the purposes of this example we will consider a local environment.

#### Running PostgreSQL

Before even touching python, let's go ahead and start a Postgres instance.
If you'd like, you can run the following to clone and enter the `tembo-py` repository:

```bash
git clone https://github.com/tembo-io/tembo.git
```
```bash
cd tembo/tembo-py
```

Then run the following command:

```bash
make run.postgres
```

#### Create a New Python File

```bash
touch example_tembo.py
```

#### Define Your Custom Prompt

```python
from tembo_py.rag import TemboRAG
import psycopg

# Constants
PROJECT_NAME = "test_add_prompt_template"
PROMPT_NAME = "example_prompt"
SYS_PROMPT = "System prompt text for example"
USER_PROMPT = "User prompt text for example"
CONNECTION_STRING = "postgresql://postgres:postgres@localhost:5432/postgres"

def initialize_tembo_rag(project_name, connection_string):
    """
    Initialize TemboRAG instance.
    """
    return TemboRAG(project_name=project_name, connection_string=connection_string)

def add_prompt_template_to_db():
    # Initialize TemboRAG instance
    tembo_rag = initialize_tembo_rag(PROJECT_NAME, CONNECTION_STRING)

    # Add the prompt template
    tembo_rag.add_prompt_template(PROMPT_NAME, SYS_PROMPT, USER_PROMPT)

    # Connect to the database to verify the insertion
    with psycopg.connect(CONNECTION_STRING) as conn:
        with conn.cursor() as cur:
            cur.execute(
                "SELECT prompt_type, sys_prompt, user_prompt FROM vectorize.prompts WHERE prompt_type = %s",
                (PROMPT_NAME,)
            )
            result = cur.fetchone()

    if result:
        print(f"Prompt '{PROMPT_NAME}' successfully added to the database.")
        print(f"System Prompt: {result[1]}")
        print(f"User Prompt: {result[2]}")
    else:
        print("Failed to add the prompt to the database.")

if __name__ == "__main__":
    add_prompt_template_to_db()
```

#### Executing the Python File and Confirming Success

```bash
python3 example_tembo.py
```

```bash
psql -h localhost -p 5432 -U postgres -W
```

```sql
CREATE EXTENSION vectorize CASCADE;
```

Run the following SELECT statement to confirm your new addition:

```sql
SELECT * FROM vectorize.prompts;
```


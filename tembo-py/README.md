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

#### Create a New Python File

```bash
touch example_tembo.py
```

#### Define Your Custom Prompt



#### Running PostgreSQL

```bash
make run.postgres
```

#### Executing the Python File and Confirming Success

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


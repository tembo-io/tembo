from tembo_py.rag import TemboRAG
from unittest.mock import patch



def test_rag():
    ctrl = TemboRAG("test", chunk_size=201)
    chunks = ctrl.prepare_from_directory("tests/fixtures")
    assert len(chunks) in [10, 11]


def test_prepare_bind_params():
    agent_name = "my-test-agent"
    ctrl = TemboRAG(agent_name)

    query = "hello world"
    chat_model = "gpt-3.5-turbo"
    prompt_template = "test_prompt_template"
    num_context = 5
    force_trim = True
    api_key = "test_key"

    q, bind_params = ctrl._prepare_query_params(
        query, chat_model, prompt_template, num_context, force_trim, api_key
    )

    assert (
        q
        == "SELECT vectorize.rag(agent_name => %s,query => %s,chat_model => %s,task => %s,api_key => %s,num_context => %s,force_trim => %s);"
    )
    assert len(bind_params) == 7
    assert bind_params[0] == agent_name
    assert bind_params[1] == query
    assert bind_params[2] == chat_model
    assert bind_params[3] == prompt_template
    assert bind_params[4] == api_key
    assert bind_params[5] == num_context
    assert bind_params[6] == force_trim

def test_add_prompt_template():
    project_name = "test_add_prompt_template"
    prompt_name = "example_prompt123"
    sys_prompt = "System prompt text for example123"
    user_prompt = "User prompt text for example123"
    connection_string = "postgresql://postgres:postgres@localhost:5432/postgres"

    # Init TemboRAG instance
    ctrl = TemboRAG(project_name=project_name, connection_string=connection_string)

    # Execute the function under test
    ctrl.add_prompt_template(prompt_name, sys_prompt, user_prompt)

    # Verify the record was inserted correctly by establishing a new database connection for verification
    import psycopg
    with psycopg.connect(connection_string) as conn:
        with conn.cursor() as cur:
            cur.execute("SELECT prompt_type, sys_prompt, user_prompt FROM vectorize.prompts WHERE prompt_type = %s", (prompt_name,))
            result = cur.fetchone()

    # Assertions to verify that the data was inserted as expected
    assert result is not None, "No record found in the database"
    assert result[0] == prompt_name, "prompt_type does not match"
    assert result[1] == sys_prompt, "sys_prompt does not match"
    assert result[2] == user_prompt, "user_prompt does not match"


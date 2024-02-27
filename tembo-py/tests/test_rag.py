from tembo_py.rag import TemboRAG


def test_rag():
    ctrl = TemboRAG("test", chunk_size=201)
    chunks = ctrl.prepare_from_directory("tests/fixtures")
    assert len(chunks) == 10


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

from tembo_py.rag import TemboRAGcontroller


def test_rag():
    ctrl = TemboRAGcontroller("test", chunk_size=201)
    chunks = ctrl.prepare_from_directory("tests/fixtures")
    assert len(chunks) == 10

from locust import HttpUser, task, between


class QuickstartUser(HttpUser):
    wait_time = between(1, 1.5)

    @task
    def read(self):
        self.client.get("")

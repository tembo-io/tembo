helm template --set dataPlaneEventsQueue=fake_queue --set controlPlaneEventsQueue=fake_queue --set secret.postgresConnectionString=not_a_real_connection_string .

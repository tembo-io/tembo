FROM gcr.io/distroless/static:nonroot
COPY --chown=nonroot:nonroot ./controller /app/
EXPOSE 8080
ENTRYPOINT ["/app/controller"]

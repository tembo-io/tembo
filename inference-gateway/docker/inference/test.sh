curl http://localhost:11434/api/generate -d '{
  "model": "llama3.2:1b",
  "prompt":"Why is the sky blue?",
  "stream": false
}'



curl http://localhost:11434/api/pull -d '{"model": "llama3.2"}'



curl http://localhost:11434/api/embeddings -d '{
  "model": "llama3.2:1b",
  "prompt": "Llamas are members of the camelid family"
}'
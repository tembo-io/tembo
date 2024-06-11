# Description: This script creates a mock server that listens for POST requests and returns a fixed response.

from http.server import BaseHTTPRequestHandler, HTTPServer
import json

fixed_response = {
  "choices": [
    {
      "finish_reason": "stop",
      "index": 0,
      "logprobs": None,
      "message": {
        "content": "The quick brown fox jumped over the lazy dogs",
        "role": "assistant",
        "tool_calls": []
      },
      "stop_reason": None
    }
  ],
  "created": 1717782843,
  "id": "cmpl-80a64c0798c24ce19433c36fcd3920f6",
  "model": None,
  "object": "chat.completion",
  "usage": {
    "completion_tokens": 163,
    "prompt_tokens": 5,
    "total_tokens": 168

  }
}

class SimpleHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        content_length = int(self.headers['Content-Length'])
        post_data = self.rfile.read(content_length)
        post_body = json.loads(post_data.decode('utf-8'))
        model_value = post_body['model']
        fixed_response["model"] = model_value

        self.send_response(200)
        self.send_header('Content-type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps(fixed_response).encode('utf-8'))

server_port = 8000
server_address = ('', server_port)

if __name__ == '__main__':
    httpd = HTTPServer(server_address, SimpleHandler)
    print(f"Starting server on port {server_port}...")
    httpd.serve_forever()

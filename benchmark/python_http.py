#!/usr/bin/env python3
"""
Python HTTP server matching the Rust example format
Usage: python3 python_http.py [port]
"""
import asyncio
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler
import threading

# Match the exact HTML responses from your Rust example
HTML_200 = """
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>EPOLL WORKER!</title>
  </head>
  <body>
    <h1>Hello!</h1>
    <p>Request sent from Epoll-worker</p>
  </body>
</html>
"""

HTML_404 = """
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>EPOLL WORKER!</title>
  </head>
  <body>
    <h1>Oops!</h1>
    <p>Sorry, I don't know what you're asking for.</p>
  </body>
</html>
"""

class HttpHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        # Match your Rust server's logic exactly
        if self.path == "/":
            status_line = "HTTP/1.1 200 OK"
            contents = HTML_200
        else:
            status_line = "HTTP/1.1 404 NOT FOUND"
            contents = HTML_404
        
        # Send the exact same format as your Rust server
        content_bytes = contents.encode('utf-8')
        
        self.send_response(200 if "200" in status_line else 404)
        self.send_header('Content-Length', str(len(content_bytes)))
        self.end_headers()
        self.wfile.write(content_bytes)
    
    def log_message(self, format, *args):
        # Suppress logging for cleaner benchmark output
        pass

def run_server(port):
    server = HTTPServer(('127.0.0.1', port), HttpHandler)
    print(f"Python HTTP server starting on port {port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("Python server shutting down...")
        server.shutdown()

if __name__ == '__main__':
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    run_server(port)
#!/usr/bin/env node
/**
 * Node.js HTTP server matching the Rust example format
 * Usage: node node_http.js [port]
 */
const http = require('http');

const port = process.argv[2] || 8080;

// Match the exact HTML responses from your Rust example
const HTML_200 = `
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
`;

const HTML_404 = `
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
`;

const server = http.createServer((req, res) => {
    // Match your Rust server's logic exactly
    let statusCode, contents;

    if (req.method === 'GET' && req.url === '/') {
        statusCode = 200;
        contents = HTML_200;
    } else if (req.method === 'GET') {
        statusCode = 404;
        contents = HTML_404;
    } else {
        statusCode = 400;
        contents = HTML_404;
    }

    // Send the exact same format as your Rust server
    const contentBuffer = Buffer.from(contents, 'utf-8');

    res.writeHead(statusCode, {
        'Content-Length': contentBuffer.length
    });
    res.end(contentBuffer);
});

server.maxConnections = 2000;
server.timeout = 5000;
server.keepAliveTimeout = 1000;

server.listen(port, '127.0.0.1', () => {
    console.log(`Node.js HTTP server listening on 127.0.0.1:${port}`);
});

// Graceful shutdown
process.on('SIGINT', () => {
    console.log('\nShutting down Node.js server...');
    server.close(() => {
        process.exit(0);
    });
});
const { test } = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const { completionUrl, streamCompletion } = require('../provider');

async function withServer(handler, run) {
  const server = http.createServer(handler);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    return await run(`http://127.0.0.1:${server.address().port}`);
  } finally {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
}

test('remote providers require HTTPS while local Ollama may use HTTP', () => {
  assert.equal(completionUrl('http://127.0.0.1:11434/v1').pathname, '/v1/chat/completions');
  assert.equal(completionUrl('https://example.com/v1/').pathname, '/v1/chat/completions');
  assert.throws(() => completionUrl('http://example.com/v1'), /HTTPS/);
});

test('streams SSE frames across network chunks', async () => {
  await withServer((request, response) => {
    assert.equal(request.method, 'POST');
    response.writeHead(200, { 'Content-Type': 'text/event-stream' });
    response.write('data: {"choices":[{"delta":{"content":"hel');
    response.write('lo"}}]}\r\n');
    response.end('data: {"choices":[{"delta":{"content":" world"}}]}\n\ndata: [DONE]\n');
  }, async baseUrl => {
    const chunks = [];
    await streamCompletion({ url: completionUrl(baseUrl), model: 'test', messages: [], signal: new AbortController().signal, onDelta: chunk => chunks.push(chunk) });
    assert.deepEqual(chunks, ['hello', ' world']);
  });
});

test('reports provider errors without exposing the API key', async () => {
  await withServer((_request, response) => {
    response.writeHead(503);
    response.end('private error body');
  }, async baseUrl => {
    await assert.rejects(streamCompletion({ url: completionUrl(baseUrl), model: 'test', messages: [], apiKey: 'x'.repeat(32), signal: new AbortController().signal, onDelta() {} }), /HTTP 503/);
  });
});

test('cancels a response that is still streaming', async () => {
  await withServer((_request, response) => {
    response.writeHead(200, { 'Content-Type': 'text/event-stream' });
    response.write('data: {"choices":[{"delta":{"content":"first"}}]}\n');
  }, async baseUrl => {
    const controller = new AbortController();
    const chunks = [];
    await assert.rejects(streamCompletion({ url: completionUrl(baseUrl), model: 'test', messages: [], signal: controller.signal, onDelta: chunk => { chunks.push(chunk); controller.abort(); } }), /abort/i);
    assert.deepEqual(chunks, ['first']);
  });
});

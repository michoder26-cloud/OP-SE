function completionUrl(baseUrl) {
  const url = new URL(`${baseUrl.replace(/\/+$/, '')}/chat/completions`);
  const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname);
  if (url.protocol !== 'https:' && !(url.protocol === 'http:' && loopback)) {
    throw new Error('Use HTTPS for remote model providers. HTTP is allowed only for a local provider.');
  }
  return url;
}

async function streamCompletion({ url, model, messages, apiKey, signal, onDelta, maxTokens }) {
  const response = await fetch(url, {
    method: 'POST',
    signal,
    headers: {
      'Content-Type': 'application/json',
      ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {})
    },
    body: JSON.stringify({ model, stream: true, messages, ...(maxTokens ? { max_tokens: maxTokens } : {}) })
  });
  if (!response.ok) throw new Error(`Provider returned HTTP ${response.status}`);
  if (!response.body) throw new Error('Provider returned no response body');

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  function consume(line) {
    if (!line.trimStart().startsWith('data:')) return;
    const data = line.trimStart().slice(5).trim();
    if (!data || data === '[DONE]') return;
    let frame;
    try { frame = JSON.parse(data); }
    catch { throw new Error('Provider returned an invalid streaming frame'); }
    if (frame.error) throw new Error(`Provider error: ${frame.error.message || 'unknown error'}`);
    const delta = frame.choices?.[0]?.delta?.content;
    if (typeof delta === 'string' && delta) onDelta(delta);
  }

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split(/\r?\n/);
      buffer = lines.pop() || '';
      for (const line of lines) consume(line);
    }
    buffer += decoder.decode();
    if (buffer.trim()) consume(buffer);
  } finally {
    reader.releaseLock();
  }
}

module.exports = { completionUrl, streamCompletion };

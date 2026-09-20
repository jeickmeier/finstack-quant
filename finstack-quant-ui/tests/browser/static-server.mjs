import { createServer } from "node:http";
import sirv from "sirv";

/** Serve only exported files, mounted at the exact deployment path. */
export async function serveExport(directory, basePath = "", port = 0) {
  const files = sirv(directory, { dev: true });
  const requests = [];
  const server = createServer((request, response) => {
    requests.push(request.url);
    if (basePath && !request.url.startsWith(`${basePath}/`)) {
      response.writeHead(404).end();
      return;
    }
    request.url = request.url.slice(basePath.length) || "/";
    files(request, response, () => response.writeHead(404).end());
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", resolve);
  });
  return {
    url: `http://127.0.0.1:${server.address().port}${basePath}`,
    requests,
    close: () =>
      new Promise((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      ),
  };
}

import assert from "node:assert/strict";
import test from "node:test";

import worker from "./worker.mjs";

test("redirects www to the HTTPS apex while preserving path and query", async () => {
  const response = await worker.fetch(
    new Request("http://www.futuruna.com/docs/basics?source=gemini&mode=full"),
  );

  assert.equal(response.status, 301);
  assert.equal(
    response.headers.get("location"),
    "https://futuruna.com/docs/basics?source=gemini&mode=full",
  );
});

test("redirects the www homepage to the canonical homepage", async () => {
  const response = await worker.fetch(
    new Request("https://www.futuruna.com/"),
  );

  assert.equal(response.status, 301);
  assert.equal(response.headers.get("location"), "https://futuruna.com/");
});

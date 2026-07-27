// The door on /beta.
//
// A published build is not finished work and is not meant to be found by
// everyone; this asks for a password before serving anything under /beta,
// including the engine itself. It is a soft gate by design — one shared
// password, no accounts — but it is a real one: the password is checked on
// Cloudflare's edge and is never part of anything the browser downloads.
//
// Set BETA_PASSWORD in the Pages project's environment variables to change it
// without a deploy.

const COOKIE = "civvis_beta";
const WEEK = 60 * 60 * 24 * 7;

/// What the cookie carries: proof of the password rather than the password.
async function token(password) {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(`civvis.ai/beta:${password}`),
  );
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/// Compare without leaking where two strings first differ.
function sameToken(a, b) {
  if (typeof a !== "string" || typeof b !== "string" || a.length !== b.length) return false;
  let differences = 0;
  for (let i = 0; i < a.length; i++) differences |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return differences === 0;
}

function cookieValue(request, name) {
  const header = request.headers.get("Cookie") || "";
  for (const part of header.split(";")) {
    const [key, ...rest] = part.trim().split("=");
    if (key === name) return rest.join("=");
  }
  return null;
}

function askForIt(wrong) {
  const page = `<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>CIVVIS — beta</title>
<style>
  :root { color-scheme: dark; }
  * { box-sizing: border-box; }
  body {
    margin: 0; min-height: 100vh; display: flex; align-items: center; justify-content: center;
    background: radial-gradient(ellipse at 50% 0%, #10231d 0%, #07110f 60%);
    color: #e8e2d2; font-family: Georgia, "Times New Roman", serif; padding: 24px;
  }
  form { width: 100%; max-width: 340px; text-align: center; }
  h1 { margin: 0; font-size: 34px; font-weight: 400; letter-spacing: 0.34em; text-indent: 0.34em; color: #d7b66a; }
  p { margin: 10px 0 30px; font-size: 12px; letter-spacing: 0.2em; text-transform: uppercase; color: #6f8279; }
  input {
    width: 100%; padding: 13px 16px; font-size: 17px; font-family: inherit; text-align: center;
    letter-spacing: 0.3em; color: #e8e2d2; background: #0d1a17;
    border: 1px solid #24413a; border-radius: 7px; outline: none;
  }
  input:focus { border-color: #d7b66a; }
  button {
    width: 100%; margin-top: 12px; padding: 12px; font-size: 13px; font-family: inherit;
    letter-spacing: 0.24em; text-transform: uppercase; cursor: pointer;
    color: #15221c; background: #d7b66a; border: 0; border-radius: 7px;
  }
  button:hover { background: #f6e4ac; }
  .wrong { margin-top: 16px; font-size: 12px; letter-spacing: 0.14em; color: #c98b7a; }
</style></head>
<body>
  <form method="POST">
    <h1>CIVVIS</h1>
    <p>Beta build</p>
    <input name="password" type="password" autofocus autocomplete="current-password"
           aria-label="Password" placeholder="password">
    <button type="submit">Enter</button>
    ${wrong ? '<div class="wrong">That is not the password.</div>' : ""}
  </form>
</body></html>`;
  return new Response(page, {
    status: wrong ? 401 : 200,
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store",
    },
  });
}

export async function onRequest(context) {
  const { request, next, env } = context;
  const password = env.BETA_PASSWORD || "2008";
  const expected = await token(password);

  if (sameToken(cookieValue(request, COOKIE), expected)) {
    const response = await next();
    // A build behind a password should not turn up in a search result.
    const headers = new Headers(response.headers);
    headers.set("X-Robots-Tag", "noindex");
    return new Response(response.body, { status: response.status, headers });
  }

  if (request.method === "POST") {
    const form = await request.formData().catch(() => null);
    if (form && form.get("password") === password) {
      return new Response(null, {
        status: 303,
        headers: {
          Location: new URL(request.url).pathname,
          "Set-Cookie":
            `${COOKIE}=${expected}; Path=/beta; Max-Age=${WEEK}; ` +
            "HttpOnly; Secure; SameSite=Lax",
          "Cache-Control": "no-store",
        },
      });
    }
    return askForIt(true);
  }

  return askForIt(false);
}

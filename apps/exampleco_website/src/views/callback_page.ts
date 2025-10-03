export function renderCallbackPage(accessToken: string): string {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Completing OAuth</title>
</head>
<body>
  <script>
    sessionStorage.setItem('cubby_access_token', ${JSON.stringify(accessToken)});
    window.location.href = '/';
  </script>
</body>
</html>`
}


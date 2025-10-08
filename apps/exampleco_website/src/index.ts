<<<<<<< HEAD
import { Hono } from "hono";
import oauthRoutes from "./routes/oauth_routes";
import apiRoutes from "./routes/api_routes";
import { renderHomePage } from "./views/home_page";
=======
import { Hono } from 'hono'
import oauthRoutes from './routes/oauth_routes'
import apiRoutes from './routes/api_routes'
import { renderHomePage } from './views/home_page'
import { renderMcpPage } from './views/mcp_page'
>>>>>>> 804745c (feat(server): Added MCP server)

type Bindings = Env;
type Variables = {
  secure: boolean;
};

export type { Bindings, Variables };

const app = new Hono<{ Bindings: Bindings; Variables: Variables }>();

app.get("/", (c) => {
  return c.html(renderHomePage(c.env.CUBBY_API_URL));
});

<<<<<<< HEAD
app.route("/", oauthRoutes);
app.route("/api", apiRoutes);
=======
app.get('/mcp-demo', (c) => {
  return c.html(renderMcpPage(c.env.CUBBY_API_URL))
})

app.route('/', oauthRoutes)
app.route('/api', apiRoutes)
>>>>>>> 804745c (feat(server): Added MCP server)

export default app;

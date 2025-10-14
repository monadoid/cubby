import { Hono } from 'hono';
type Bindings = Env;
type Variables = {
    secure: boolean;
};
export type { Bindings, Variables };
declare const app: Hono<{
    Bindings: Bindings;
    Variables: Variables;
}, import("hono/types").BlankSchema, "/">;
export default app;
//# sourceMappingURL=index.d.ts.map
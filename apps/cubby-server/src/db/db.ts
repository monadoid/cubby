import Env = Cloudflare.Env;
import { drizzle } from 'drizzle-orm/neon-http';
import {users} from "./schema";



export default {
    async fetch(request: Request, env: Env) {
        const db = drizzle({connection: env.DATABASE_URL, casing: "snake_case"});
        const result = await db.select().from(users);
        return Response.json(result);
    },
};

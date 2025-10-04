import { drizzle } from "drizzle-orm/neon-http";

export const createDbClient = (db_url: string) => {
  return drizzle({
    connection: db_url,
    casing: "snake_case",
  });
};

export type DbClient = ReturnType<typeof createDbClient>;

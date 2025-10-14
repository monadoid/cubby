import { sql } from "drizzle-orm";
import { index, pgTable, text, timestamp, uuid } from "drizzle-orm/pg-core";
import { customAlphabet, nanoid } from "nanoid";

const deviceId = customAlphabet("abcdefghijklmnopqrstuvwxyz0123456789", 15);
export const generateDeviceId = () => deviceId();

export const users = pgTable("users_table", {
  id: uuid().defaultRandom().primaryKey(),

  authId: text().notNull().unique(),

  email: text().notNull().unique(),

  createdAt: timestamp({ withTimezone: true, mode: "string" })
    .default(sql`(now() AT TIME ZONE 'utc'::text)`)
    .notNull(),

  updatedAt: timestamp({ withTimezone: true, mode: "string" })
    .default(sql`(now() AT TIME ZONE 'utc'::text)`)
    .notNull()
    .$onUpdate(() => sql`(now() AT TIME ZONE 'utc'::text)`),
});

export const devices = pgTable(
  "devices",
  {
    id: text()
      .primaryKey()
      .$defaultFn(() => generateDeviceId()),
    userId: uuid()
      .notNull()
      .references(() => users.id, { onDelete: "cascade" }),

    createdAt: timestamp({ withTimezone: true, mode: "string" })
      .default(sql`(now() AT TIME ZONE 'utc'::text)`)
      .notNull(),

    updatedAt: timestamp({ withTimezone: true, mode: "string" })
      .default(sql`(now() AT TIME ZONE 'utc'::text)`)
      .notNull()
      .$onUpdate(() => sql`(now() AT TIME ZONE 'utc'::text)`),
  },
  (table) => {
    return {
      userIdIdx: index("devices_user_id_idx").on(table.userId),
      createdAtIdx: index("devices_created_at_idx").on(table.createdAt),
    };
  },
);

// Inferred types
export type User = typeof users.$inferSelect;
export type NewUser = typeof users.$inferInsert;

export type Device = typeof devices.$inferSelect;
export type NewDevice = typeof devices.$inferInsert;

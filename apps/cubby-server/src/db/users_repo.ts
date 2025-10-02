import type { z } from 'zod';
import { users } from './schema';
import type { DbClient } from './client';
import {createInsertSchema} from "drizzle-zod";

const baseUserInsertSchema = createInsertSchema(users, {
    authId: (schema) => schema.min(1, 'authId is required'),
    email: (schema) => schema.email('Invalid email address'),
});

export const createUserSchema = baseUserInsertSchema.pick({
    id: true,
    authId: true,
    email: true,
});


export type CreateUserInput = z.infer<typeof createUserSchema>;

export async function createUser(db: DbClient, input: CreateUserInput) {
    const [user] = await db
        .insert(users)
        .values({ id: input.id, authId: input.authId, email: input.email })
        .returning();
    return user;
}

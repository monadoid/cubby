import { z } from 'zod';
import { createInsertSchema } from 'drizzle-zod';

import type { DbClient } from './client';
import { devices } from './schema';

const baseDeviceInsertSchema = createInsertSchema(devices, {
    id: () => z.string().min(1, 'id is required'),
    userId: () => z.string().uuid('Invalid user id'),
});

export const createDeviceSchema = baseDeviceInsertSchema.pick({
    id: true,
    userId: true,
});

export type CreateDeviceInput = z.infer<typeof createDeviceSchema>;

export async function createDevice(db: DbClient, input: CreateDeviceInput) {
    const [device] = await db
        .insert(devices)
        .values({ id: input.id, userId: input.userId })
        .returning();
    return device;
}

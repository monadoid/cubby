ALTER TABLE "users_table" ADD COLUMN "auth_id" text NOT NULL;--> statement-breakpoint
ALTER TABLE "users_table" ADD CONSTRAINT "users_table_auth_id_unique" UNIQUE("auth_id");
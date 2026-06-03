#!/bin/python3

"""
Copy all files from seaweedfs to the new postgres bytea format

NOTE/DISCLAIMER: Made with help of `gemini-cli` and then manually reviewed (as its mostly just boilerplate)
"""

import boto3
import ruamel.yaml
import asyncio
import asyncpg
import random
import string
import sys

# Max size for blob column as per src/migrations/tenant_kv_add_bytea.rs (512KB)
MAX_BLOB_SIZE = 524288

def generate_id(length=64):
    """Generate a random alphanumeric string of a given length."""
    return ''.join(random.choices(string.ascii_letters + string.digits, k=length))

async def migrate():
    # Load configuration
    try:
        with open("config.yaml") as f:
            yaml = ruamel.yaml.YAML(typ='safe', pure=True)
            config = yaml.load(f)
    except FileNotFoundError:
        print("Error: config.yaml not found.")
        sys.exit(1)

    # S3 setup
    try:
        s3_config = config["object_storage"]
        endpoint = s3_config["endpoint"]
        access_key = s3_config["access_key"]
        secret_key = s3_config["secret_key"]
        secure = s3_config["secure"]

        s3_client = boto3.client(
            "s3",
            endpoint_url=f"http://{endpoint}" if not secure else f"https://{endpoint}",
            aws_access_key_id=access_key,
            aws_secret_access_key=secret_key,
        )
    except KeyError as e:
        print(f"Error: Missing key in object_storage config: {e}")
        sys.exit(1)

    # Postgres setup
    try:
        postgres_url = config["meta"]["postgres_url"]
        conn = await asyncpg.connect(postgres_url)
    except Exception as e:
        print(f"Error: Could not connect to PostgreSQL: {e}")
        sys.exit(1)

    file_count = 0
    migrated_count = 0
    big_files = []

    print("Starting migration from SeaweedFS to PostgreSQL...")

    try:
        object_paginator = s3_client.get_paginator('list_objects_v2')
        # Bucket name from the original script
        bucket_name = "antiraid.guilds"

        for object_page in object_paginator.paginate(Bucket=bucket_name):
            for fileobj in object_page.get("Contents", []):
                s3_key: str = fileobj["Key"]
                size = fileobj["Size"]
                file_count += 1

                # Parse tenant_id and file_name from key: {tenant_id}/{file_name}
                if "/" not in s3_key:
                    print(f"Warning: Skipping unexpected S3 key format: {s3_key}")
                    continue

                parts = s3_key.split("/", 1)
                tenant_id = parts[0]
                file_name = parts[1]

                if size > MAX_BLOB_SIZE:
                    print(f"Skipping {s3_key} (Size: {size} bytes exceeds limit)")
                    big_files.append(f"{s3_key} ({size} bytes)")
                    continue

                print(f"Migrating {s3_key}...")

                # Fetch object content
                try:
                    response = await asyncio.to_thread(
                        s3_client.get_object, Bucket=bucket_name, Key=s3_key
                    )
                    blob_data = await asyncio.to_thread(response['Body'].read)
                except Exception as e:
                    print(f"Error: Failed to fetch {s3_key} from S3: {e}")
                    continue

                # Generate random 64-char ID
                new_id = generate_id()

                # Upsert into tenant_kv
                # We assume owner_type 'guild' and scope '' (default)
                # value is set to 'null'::jsonb as it's a blob-only entry from S3
                try:
                    await conn.execute(
                        """
                        INSERT INTO tenant_kv (id, owner_id, owner_type, key, scope, blob, value)
                        VALUES ($1, $2, 'guild', $3, '', $4, 'null'::jsonb)
                        ON CONFLICT (owner_id, owner_type, key, scope) 
                        DO UPDATE SET blob = EXCLUDED.blob, last_updated_at = NOW()
                        """,
                        new_id, tenant_id, file_name, blob_data
                    )
                    migrated_count += 1
                except Exception as e:
                    print(f"Error: Failed to insert {s3_key} into database: {e}")

        print("\nMigration finished!")
        print(f"Total files processed: {file_count}")
        print(f"Total files migrated: {migrated_count}")

        if big_files:
            print("\nFiles ignored due to size limit (> 512KB):")
            for bf in big_files:
                print(f"  - {bf}")

    finally:
        await conn.close()

if __name__ == "__main__":
    asyncio.run(migrate())
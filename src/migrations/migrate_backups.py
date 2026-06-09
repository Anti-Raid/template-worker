#!/bin/python3

"""
Copy all files from seaweedfs to the new postgres bytea format
"""

import boto3
import ruamel.yaml 
import asyncio
import asyncpg
import random
import string
import sys 
import json

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from types_boto3_s3 import S3Client


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
        if "object_storage" not in config:
            print("WARNING: No object storage set in config, not migrating backups")
            return
        
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
        sys.exit(34)

    # Postgres setup
    conn: asyncpg.Connection
    try:
        postgres_url = config["meta"]["postgres_url"]
        conn = await asyncpg.connect(postgres_url)
        await conn.set_type_codec(
            'jsonb',
            encoder=json.dumps,
            decoder=json.loads,
            schema='pg_catalog'
        )
    except Exception as e:
        print(f"Error: Could not connect to PostgreSQL: {e}")
        sys.exit(1)

    async with conn.transaction():
        # Move backups
        rows = await conn.fetch("SELECT id, owner_id, key, value FROM tenant_kv WHERE scope = 'builtins.backups.metadata' AND blob IS NULL AND owner_type = 'guild'")
        for row in rows:
            print(f"Migrating row {row}")
            keyid = row["id"]
            owner_id = row["owner_id"]
            filename = row["value"]["filename"]
            resp = s3_client.get_object(Bucket="antiraid.guilds", Key=f"{owner_id}/{filename}")
            blob = resp["Body"].read() # Note to self: if this fails, 
            if len(blob) > MAX_BLOB_SIZE:
                raise RuntimeError(f"{filename} (Size: {len(blob)} bytes exceeds limit)")
            await conn.execute("UPDATE tenant_kv SET blob = $2, value = $3, key = $4 WHERE id = $1", keyid, blob, {"Map": [[{"Text": "createdby"}, "Null"]]}, filename)

if __name__ == "__main__":
    asyncio.run(migrate())
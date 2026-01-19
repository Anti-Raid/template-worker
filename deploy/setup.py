#!/usr/bin/env python3
"""Sets up non-docker config"""
import os
import secrets
import base64
import json
import pathlib
import argparse

parser = argparse.ArgumentParser(
    prog='setup.py',
    description='Sets up Anti-Raid configuration files',
    epilog='https://github.com/anti-raid/template-worker')
parser.add_argument('--output_path', type=str, default="./deploy/dockerconf", help='Output path for configuration files')

# Docker
parser.add_argument('--docker', action='store_true', help='Setup for docker (default)')
parser.add_argument('--no-docker', dest='docker', action='store_false', help='Setup for non-docker')
parser.set_defaults(docker=True)

# Seaweed
parser.add_argument('--local_fs_path', type=str, default='', help="If this is set, use local filesystem storage at the given path instead of SeaweedFS")

args = parser.parse_args()

if args.docker and args.output_path != "./deploy/dockerconf":
    print("When using --docker, output_path must be ./deploy/dockerconf")
    exit(1)

os.mkdir(args.output_path) if not os.path.exists(args.output_path) else None
if args.local_fs_path:
    print("Using local filesystem storage at", args.local_fs_path)
    os.mkdir(args.local_fs_path) if not os.path.exists(args.local_fs_path) else None

if args.docker:
    print("Setting up for Docker...")
else:
    print("Setting up for non-Docker...")

# Get bot token, client id and client secret
def get_var(prompt: str, env_var: str) -> str:
    """Gets a variable from the user or environment"""
    value = os.getenv(env_var)
    if not value:
        while True:
            value = input(f"{prompt}: ")

            if value:
                break
            print(f"{prompt} cannot be empty")

    if not value:
        print(f"{prompt} cannot be empty")
        exit(1)

    return value.replace(" ", "").replace("\n", "")

realtoken = get_var("Bot Token", "BOT_TOKEN")
client_id = get_var("Client ID", "CLIENT_ID")
client_secret = get_var("Client Secret", "CLIENT_SECRET")
sandwich_alert_webhook = get_var("Alert Webhook (for sandwich)", "SANDWICH_ALERT_WEBHOOK")
root_users = get_var("Main/Root Users (comma separated)", "ROOT_USERS").split(",")

# Process root users
root_user_str = ""
for i in range(len(root_users)):
    user = root_users[i].strip()
    root_user_str += f'    - "{user}" # Root User {i+1}\n'

# Create some needed secrets
faketoken = f"{base64.b64encode(client_id.encode()).decode()}.{secrets.token_urlsafe(32)}.{secrets.token_urlsafe(32)}"
s3_access_key = secrets.token_urlsafe(32)
s3_secret_key = secrets.token_urlsafe(64)
vapid_public_key = secrets.token_urlsafe(32)
vapid_private_key = secrets.token_urlsafe(64)
dp_secret = secrets.token_urlsafe(128)
default_error_channel = get_var("Default Error Channel ID", "DEFAULT_ERROR_CHANNEL")

# Determine URLs based on docker or not
sandwich_url = "http://sandwich:29334" if args.docker else "http://localhost:3600"
proxy_url = "http://nirn_proxy:3221" if args.docker else "http://localhost:3221"
pg_url = "postgres://antiraid:AnTiRaId123!@postgres:5432/antiraid" if args.docker else "postgres:///antiraid"
seaweed_endpoint = "seaweed:8333" if args.docker else "localhost:8333"
seaweed_cdn_endpoint = "$DOCKER:localhost:5601" if args.docker else "localhost:8333"

# Determine config file names based on docker or not
tw_config_filename = "config.docker.yaml" if args.docker else f"{args.output_path}/tw_config.yaml"
nirn_secrets_filename = "secrets.docker.json" if args.docker else "nirn_secrets.json"
sandwich_url = "sandwich.docker.yaml" if args.docker else "sandwich.yaml"
s3_json_filename = "deploy/docker/seaweed/s3.json" if args.docker else f"{args.output_path}/seaweed_s3.json"

SEAWEED_DATA = """
object_storage:
  type: s3-like # Type of object storage. Can be s3-like or local
  base_path: 
  endpoint: {seaweed_endpoint} # Endpoint for Seaweed
  cdn_endpoint: {seaweed_cdn_endpoint} # CDN Endpoint for Seaweed
  access_key: {s3_access_key} # Access Key for Seaweed
  secret_key: {s3_secret_key} # Secret Key for Seaweed
  secure: false
  cdn_secure: false
"""
if args.local_fs_path:
    SEAWEED_DATA = f"""
object_storage:
  type: local # Type of object storage. Can be s3-like or local
  base_path: {args.local_fs_path} # Base path for local storage
"""

BASE_CONFIG_FILE = f"""
discord_auth:
  token: "{faketoken}" # Discord Bot Token
  client_id: "{client_id}" # Discord Client ID - Main
  client_secret: "{client_secret}" # Discord Client Secret - Main
  root_users:
{root_user_str}
  allowed_redirects: # Change afterwards if desired
    - http://localhost:5173/authorize
    - http://localhost:5174/authorize
    - https://v6-beta.antiraid.xyz/authorize
    - https://antiraid.xyz/authorize

sites:
  frontend: https://antiraid.xyz # Staging value
  api: http://localhost:5600 # Staging value
  docs: https://docs.antiraid.xyz # Documentation URL

servers:
  main: "1064135068928454766" # Main Server ID

meta:
  postgres_url: {pg_url} # Postgres URL
  proxy: {proxy_url} # Proxy URL
  support_server_invite: https://discord.gg/9BJWSrEBBJ
  sandwich_http_api: {sandwich_url}
  default_error_channel: "{default_error_channel}" # Change this

{SEAWEED_DATA}

addrs:
  template_worker: http://0.0.0.0:60000
  mesophyll_server: http://127.0.0.1:70000
"""

print(f"Saving {tw_config_filename}")
with open(tw_config_filename, "w") as f:
    f.write(BASE_CONFIG_FILE)

# Save secrets.docker.json with <faketoken>:<token>
nirn_secrets = {
    faketoken: realtoken,
}

print(f"Saving {nirn_secrets_filename}")
with open(f"{args.output_path}/{nirn_secrets_filename}", "w") as f:
    json.dump(nirn_secrets, f, indent=4)

SANDWICH_YAML = f"""
identify:
    url: ""
    headers: {{}}
producer:
    type: websocket
    configuration:
        address: 0.0.0.0:3600
        expectedtoken: "{faketoken}"
http:
    oauth:
        clientid: "{client_id}"
        clientsecret: "{client_secret}"
        endpoint:
            authurl: https://discord.com/api/oauth2/authorize?prompt=none
            deviceauthurl: ""
            tokenurl: https://discord.com/api/oauth2/token
            authstyle: 0
        redirecturl: http://splashtail-sandwich.antiraid.xyz/callback
        scopes:
            - identify
            - email
    user_access: # Change this later
        - "USER1"
        - "USER2"
        - "USER3"
webhooks:
    - {sandwich_alert_webhook}
managers:
    - identifier: antiraid
      virtual_shards:
        enabled: true
        count: 30
        dm_shard: 0
      producer_identifier: antiraid_producer
      friendly_name: Anti Raid
      token: {realtoken}
      auto_start: true
      disable_trace: true
      bot:
        default_presence:
            status: online
            activities:
                - timestamps: null
                  applicationid: null
                  party: null
                  assets: null
                  secrets: null
                  flags: null
                  name: Listening to development of Anti-Raid v6 
                  url: null
                  details: null
                  state: Listening to development of Anti-Raid v6
                  type: 1
                  instance: null
                  createdat: null
            since: 0
            afk: false
        intents: 20031103
        chunk_guilds_on_startup: false
      caching:
        cache_users: true
        cache_members: true
        store_mutuals: true
      events:
        event_blacklist: []
        produce_blacklist: []
      messaging:
        client_name: antiraid
        channel_name: sandwich
        use_random_suffix: true
      sharding:
        auto_sharded: true
        shard_count: 0
        shard_ids: ""
"""

# Save to sandwich.docker.yaml
print(f"Saving {sandwich_url}")
with open(f"{args.output_path}/{sandwich_url}", "w") as f:
    f.write(SANDWICH_YAML)

# Finally seaweed
if args.docker:
    os.mkdir("deploy/docker/seaweed") if not os.path.exists("deploy/docker/seaweed") else None

s3_json = {
  "identities":  [
    {
      "name":  "antiraid",
      "credentials":  [
        {
          "accessKey":  s3_access_key,
          "secretKey":  s3_secret_key
        }
      ],
      "actions":  [
        "Read",
        "Write",
        "List",
        "Tagging",
        "Admin"
      ],
    },
  ],
  "accounts":  []
}

print(f"Saving {s3_json_filename}")
with open(s3_json_filename, "w") as f:
    json.dump(s3_json, f, indent=4)

if args.docker:
    print("Done! Now run `docker compose up` to start the containers.")
else:
    print("Done!")

#!/usr/bin/env python3

"""Really simple script to push a script to AntiRaid"""
from dotenv import load_dotenv

load_dotenv()  # take environment variables

import os
import requests

API_URL = "https://splashtail-staging.antiraid.xyz/"
api_token = os.getenv("API_TOKEN")
if not api_token:
    raise ValueError("API_TOKEN is not set in the environment variables")

guild_id = os.getenv("GUILD_ID")
if not guild_id:
    raise ValueError("GUILD_ID is not set in the environment variables")

res = requests.get(f"{API_URL}/guilds/{guild_id}/settings", 
    headers={
        "Authorization": api_token,
        "Content-Type": "application/json"
    },
    timeout=180
)

print(res.json())

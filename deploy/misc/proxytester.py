import discord
import logging

logging.getLogger("discord.gateway").setLevel(logging.DEBUG)

print(discord.gateway)

bot = discord.AutoShardedClient(intents=discord.Intents.default())

@bot.event
async def on_ready():
    print(bot.user.name)


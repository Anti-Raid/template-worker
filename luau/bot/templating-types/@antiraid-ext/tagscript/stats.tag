$stats: @botstats
$myembed:
    title: "Stats"
    description: "Here are some cool bot stats"
    fields:
        "Bot Name": @botusername
        "Total Users": $stats.total_users
        "Total Servers": $stats.total_guilds
        "Bot Last Started": @ts($stats.last_started_at)
        "Bot Uptime": @since($stats.last_started_at)
@reply: $myembed
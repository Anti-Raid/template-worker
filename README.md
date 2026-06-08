# template-worker

Main bot process that handles dispatching templates (and basically all of AntiRaid)

## Components

- ``master``: Contains master process specific code (such as `msyscall` which is the main API for external users to communicate with the rest of AntiRaid).
- ``mesophyll``: Contains Mesophyll, which is the main (currently gRPC-based) communication layer between the master process and all the child worker processes that actually handle templates.
- ``worker``: Contains the worker specific code (such as Luau VM management code, event dispatch, tenant state tracking code and `wsyscall` for Luau->Worker communication)
- ``geese``: Contains systems that are common to both master and worker such as stratum (gateway) client code, and state management code. Named after the Canadian Goose/Geese.

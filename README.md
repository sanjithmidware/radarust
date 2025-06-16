please add  a config file at the root in the same level as src and cargo.toml 

# Azure Blob Storage Configuration
azure:
  account: "your-account"
  access_key: "your key"
  
  container: "your container"
  path_prefix: "your prefix"
# Polling configuration
polling:
  # The duration to wait between polls, in seconds.
  duration_seconds: 300 # e.g., 5 minutes

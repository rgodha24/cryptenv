# cryptenv

A super simple env variable manager.
It encrypts and saves your environment in a JSON file at DATA_DIR/cryptenv/store.json. 
The encryption key is kept in your computers secure store using [keyring](docs.rs/keyring).
Then, by editing your cryptenv.toml file, you can set environment variables for specific projects on your computer, which are automatically changed whenever you `cd` into the project directory.

For example, if you had a directory called `~/Coding/` with this layout
```
.
├── company-project
└── personal-project
```

you would define your `cryptenv.toml` like this
```toml
dirs = ["~/Coding/"]

# Define profiles for reusable environment variable sets
[profile.cloud-dev]
AWS_REGION = "AWS_REGION_VALUE"
AWS_PROFILE = "AWS_PROFILE_VALUE"

[profile.cloudflare]
CLOUDFLARE_EMAIL = "CLOUDFLARE_EMAIL_VALUE"

# Define projects with direct variables
[project.company-project]
profiles = ["cloud-dev", "cloudflare"]  # Use profiles
vars = { CLOUDFLARE_API_TOKEN = "COMPANY_CLOUDFLARE_TOKEN" }  # Project-specific vars

# Define projects with just profiles
[project.personal-project]
profiles = ["cloudflare"]
vars = { CLOUDFLARE_API_TOKEN = "PERSONAL_CLOUDFLARE_TOKEN" }

# You can also define projects with simplified syntax that just references profiles
[project]
simple-project = ["cloud-dev"]
```

and add your variables like this:
```bash
cryptenv add COMPANY_CLOUDFLARE_TOKEN <token>
cryptenv add PERSONAL_CLOUDFLARE_TOKEN <token>
cryptenv add AWS_REGION_VALUE <region>
cryptenv add AWS_PROFILE_VALUE <profile>
cryptenv add CLOUDFLARE_EMAIL_VALUE <email>
```

You can list available profiles with `cryptenv profiles` and view variables in a profile with `cryptenv profile-vars <profile-name>`.

You can also run a command with the environment variables from a specific profile:
```bash
cryptenv run cloud-dev -- aws s3 ls
```

This will execute the command after `--` with all environment variables from the specified profile.

## installation 
note: this is very much so a work in progress. no semver guarantees!

`cargo install --git github.com/rgodha24/cryptenv`

and edit your .zshrc
```zsh
eval "$(cryptenv init zsh)"
```

the config file lives in `~/.config/cryptenv.toml`

# cryptenv

A super simple env variable manager. It saves variables using the [keyring](docs.rs/keyring) crate, which puts them into the macos keychain (or windows/linux equivalent). then, by editing your cryptenv.toml config, you can set environment variables based on your current project. projects are defined by the directory you're currently in. 

For example, if you had a directory called `~/Coding/` with this layout
```
.
├── company-project
└── personal-project
```

you would define your `cryptenv.toml` like this
```toml
dirs = ["~/Coding/"]

[projects.company-project]
CLOUDFLARE_API_TOKEN = "COMPANY_CLOUDFLARE_TOKEN"

[projects.personal-project]
CLOUDFLARE_API_TOKEN = "PERSONAL_CLOUDFLARE_TOKEN"

```

and add your variables like this `cryptenv add COMPANY_CLOUDFLARE_TOKEN <token>` and `cryptenv add PERSONAL_CLOUDFLARE_TOKEN <token>`

## installation 
note: this is very much so a work in progress, you probably shouldn't install it lol

`cargo install --git github.com/rgodha24/cryptenv`

and edit your .zshrc
```zsh
_crypt_autoload_hook () {
  cryptenv load
}
add-zsh-hook chpwd _crypt_autoload_hook
```

the config file lives in `~/.config/cryptenv.toml`

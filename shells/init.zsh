cryptenv_load () {
  eval "$(cryptenv load)"
}

add-zsh-hook chpwd cryptenv_load
cryptenv_load

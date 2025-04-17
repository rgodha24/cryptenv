cryptenv_load () {
  eval "$(cryptenv load zsh)"
}

add-zsh-hook chpwd cryptenv_load
cryptenv_load

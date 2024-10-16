cryptenv_load () {
  eval "$(cryptenv project load zsh)"
}

add-zsh-hook chpwd cryptenv_load
cryptenv_load

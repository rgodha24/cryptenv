function _cryptenv_autoload_hook() {
  eval "$(cryptenv load)"
}
add-zsh-hook chpwd _cryptenv_autoload_hook
eval $(cryptenv load)

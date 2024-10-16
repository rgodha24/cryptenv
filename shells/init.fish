function cryptenv_load
    eval (cryptenv load fish)
end

function on_directory_change --on-variable PWD
    cryptenv_load
end

cryptenv_load

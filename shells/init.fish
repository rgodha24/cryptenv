function cryptenv_load
    cryptenv load fish | source
end

function on_directory_change --on-variable PWD
    cryptenv_load
end

cryptenv_load

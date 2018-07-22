#/usr/bin/env bash

update_mizer_dir()
{
  if [ -z "$GIT_MIZER_DIR" ] && [ -d ".git-mizer" ]
  then
    GIT_MIZER_DIR=".git-mizer"
  fi
}

complete_variant()
{
  local cur;
  cur="${COMP_WORDS[COMP_CWORD]}"
  update_mizer_dir
  pushd "$GIT_MIZER_DIR/upper" > /dev/null
  COMPREPLY=($(compgen -d -- ${cur}))
  popd > /dev/null
}

_mizer_enter()
{
  local cur;
  COMPREPLY=()
  cur="${COMP_WORDS[COMP_CWORD]}"
  if [ "$COMP_CWORD" -eq 1 ]
  then
    if [ -z "$GIT_MIZER_TARGET" ]
    then
      COMPREPLY=($(compgen -d -- ${cur}))
      return 0
    else
      complete_variant
      return 0
    fi
  else
    if [ "$COMP_CWORD" -eq 2 ]
    then
      complete_variant
      return 0
    fi
  fi
  return 1
}

_mizer_first_arg_variant()
{
  local cur;
  COMPREPLY=()
  cur="${COMP_WORDS[COMP_CWORD]}"
  if [ "$COMP_CWORD" -eq 1 ]
  then
    complete_variant
    return 0
  fi
  return 1
}

complete -F _mizer_enter mizer-enter
complete -F _mizer_first_arg_variant mizer-switch
complete -F _mizer_first_arg_variant mizer-rm

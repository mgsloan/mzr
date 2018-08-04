# `mizer` / `git-mizer` project

## Motivation

The primary purpose of this project is to make it straightforward and efficient
to work on multiple checkouts of a git repository. More specifically, the plan
is to use a combination of linux features to make it so that you can have
different processes see different worktree states.

The main current goals are:

* A `mzr shell` command, which starts a shell with its own mutable snapshot of
  the repository (called a "mizer context").

* A `mzr switch` command, which switches the current shell to a different mizer
  context.

* A `mzr run` command, which creates a temporary mizer context and runs a
  command within it. This way you can run a build and continue editing your
  files.

* The different contexts should share the same git repository, rather than also
  forking the repository state.  This can either use a mechanism like `git
  worktree`, or something like the old mechanism for

## Implementation approach

This problem is quite similar to the problems solved by containerization
projects like [`docker`](https://www.docker.com/) and
[`lxc`](https://linuxcontainers.org/).  We want a degree of control and
isolation of process execution environment.  However, unless you are already
using docker for your build and execution environment, the full isolation that
docker provides would probably mostly impedes development.

Personally, I do not need or want isolation of network, ipc, host names, process
ids, or user ids. Other than the contents of mizer workdirs, I want the rest of
the filesystem to look the same as usual.

There are many similarities to docker, though. We also want to use a union
filesystem like `overlayfs` to track changes made atop snapshots. This can allow
for creating a new "fork" of the repository state to be very efficient in both
time and space.

Here's a rough sketch of how this can be achieved:

* When you run `mzr shell`, something like this happens:

  1. The process can enter into a new user namespace, where the user is treated
     like root. This allows the next step to happen without superuser
     privileges.

  2. The process can enter into a new mount namespace. Because the user is
     treated like root (in a restricted way), they can now mount an overlayfs
     filesystem for the workdir. The lower layer is a snapshot of the
     repository.

  3. `bash` or other shell gets invoked. As a child process, it will inherit the
     new user and mount namespace.

* When you run `mzr switch`, something like this happens:

  1. The workdir is unmounted. This can only work if the workdir isn't busy.
     Happily, emacs does not keep the workdir busy. When using my prototype
     scripts, if you run emacs in a mizer shell, and switch contexts, emacs will
     happily update the buffers to match the file state in the new context!

  2. The workdir is mounted with the proper overlayfs options.

* `mzr run` works very similarly to `mzr shell`.  The main point of it is a UI
  consideration - making it easy to make new contexts without needing a name.
  These contexts wouldn't show up by default in listings of contexts unless you
  asked for them.  They might expire immediately after the task exits, or they
  might potentially be garbage collected after some expirey interval.

## Other ideas / goals

Beyond the current goals listed in "Motivation", there's lots of other cool
stuff that could be done for a tool like this.  Some of them should be quite
straightforward additions:

* Often, I want to return to a branch and not need to do a big rebuild.  In
  other words, versioning build artifacts per-branch.  It should be possible to
  script this via mizer.

  - A related feature is keeping uncommitted / unstashed changes.  It should be
    possible to use mizer to keep uncommitted changes associated with a branch.

* Usage of cgroups to limit the resource consumption of mizer contexts.  This
  way you can limit the processor usage of a long running build, so that your
  other work is not impeded.

* A command to efficiently take read-only repository snapshots. On filesystems
  like `btrfs` this could be done with subvolumes or use of rsync. Just with
  `overlayfs` alone I think a lot of speed and efficiency could be gotten by
  rsyncing to a merged mount that uses an existing snapshot as the

* It should be possible to efficiently run commands against the clean repository
  state. I won't go into much of the details here (still very speculative), but
  essentially the idea is to use multiple overlayfs layers. Untracked files
  would go in a shared lower layer, while build results go in the upper layer.
  This would require mechanisms similar to the "overlayfs snapshots" project.

* It should be possible to have multiple mizer contexts open on different
  subdirs in the same shell.

* It should handle git submodules correctly and efficiently. Not trivial, but
  should be doable.

## Current state

Currently this repository just has some proof of concept bash scripts in `bin/`.
The CLI will surely change drastically, because they are pretty clunky and have
some serious problems. Rather than properly document these proof-of-concept
scripts, here's a quick demonstration of how they work:

```
mgsloan@treetop:~/proj/mizer/sandbox$ mkdir snap
mgsloan@treetop:~/proj/mizer/sandbox$ cd snap/
mgsloan@treetop:~/proj/mizer/sandbox/snap$ touch snap-file
mgsloan@treetop:~/proj/mizer/sandbox/snap$ cd ../
mgsloan@treetop:~/proj/mizer/sandbox$ cd -
/home/mgsloan/proj/mizer/sandbox/snap
mgsloan@treetop:~/proj/mizer/sandbox/snap$ mizer-enter ../work
Entering random git-mizer variant: variantXaQ
[variantXaQ] root@treetop:~/proj/mizer/sandbox# exit
exit
mgsloan@treetop:~/proj/mizer/sandbox/snap$ mizer-enter ../work alice
Entering git-mizer variant: alice
[alice] root@treetop:~/proj/mizer/sandbox# cd work
[alice] root@treetop:~/proj/mizer/sandbox/work# touch a-file
[alice] root@treetop:~/proj/mizer/sandbox/work# ls
a-file  snap-file
```

Then, in another terminal:

```
mgsloan@treetop:~/proj/mizer/sandbox/snap$ mizer-enter ../work bob
Entering git-mizer variant: bob
[bob] root@treetop:~/proj/mizer/sandbox# cd work
[bob] root@treetop:~/proj/mizer/sandbox/work# ls
snap-file
[bob] root@treetop:~/proj/mizer/sandbox/work# touch b-file
[bob] root@treetop:~/proj/mizer/sandbox/work# ls
b-file  snap-file
[bob] root@treetop:~/proj/mizer/sandbox/work# mizer-switch alice
Using first cli argument as the git-mizer variant: alice
umount: /home/mgsloan/proj/mizer/sandbox/work: target is busy.
Failed to unmount git-mizer work directory.

fuser -v output may be helpful in identifying what process is keeping it in use:

                     USER        PID ACCESS COMMAND
/home/mgsloan/proj/mizer/sandbox/work:
                     root      ..c.. bash
                     root      ..c.. bash
                     root      ..c.. grep
                     root      ..c.. grep
[bob] root@treetop:~/proj/mizer/sandbox/work# cd ../
[bob] root@treetop:~/proj/mizer/sandbox# mizer-switch alice
Using first cli argument as the git-mizer variant: alice
[alice] root@treetop:~/proj/mizer/sandbox# cd work
[alice] root@treetop:~/proj/mizer/sandbox/work# ls
a-file  snap-file
```

The first call of `mizer-switch` failed because the filesystem was busy due to
the shell keeping the mount busy.

There are numerous problems with this solution.  Here are a few:

0. Bash is awful. I started prototyping it in bash.  Now it's clear that I'm
   going to want to have logic a bit more involved than just running processes
   and a few conditionals.  So no more development will be done via bash.

1. This uses a program called `userns_child_exec`, which is an example program
   from "The Linux Programming Interface" book. A compiled version of this
   utility is included in `bin` since it is quite small. This is not a standard
   utility, and I'm not sure if the compiled binary will work on other people's
   systems. I think that `runc` / `nsenter` could be used instead, and those are
   much more commonly understood utilities.

2. It doesn't remember the snapshot dir associated with a given work dir, so you
   need to remember the association.

3. When you have multiple shells on the same mizer context mutating files
   simultaneously, overlayfs sometimes gets rather confused.  I think the
   solution to this is having the processes share mount namespace.

## Q/A

### Q: Why the name "mizer"?

A: This is a tool that will be particularly appreciated by people that are
opti-**mizers** with regards to their productivity. It's also really close to
`miser`, which, like `git`, is a pejorative noun to describe a person.


> [**git**](https://www.merriam-webster.com/dictionary/git)
>
> *British*
> : a foolish or worthless person


> [**miser**](https://www.merriam-webster.com/dictionary/miser)
>
> : a mean grasping person
>
>   * a *miser* cackling over unexpected treasure - R. T. Peterson
>
> *especially* : one who is extremely stingy with money
>
>   * a *miser* who inherited a fortune but lives in a shanty

### Q: This seems complicated. Why not just use `git worktree`?

The standard git solution to multiple checkouts is indeed to use [`git
worktree`], but it doesn't quite do what I want:

1. With multiple copies of the repository on disk, I need to be careful which
   file to edit in my text editor. Editor features that let you switch buffers
   by name, or open recently opened files become much less convenient to use.

2. Typically when you create a new worktree, depending on your build system, you
   often also need to do a full rebuild, since your new worktree is clean. This
   isn't good for productivity, it'd be more efficient to reuse your old build
   artifacts for an incremental rebuild.

3. Switching branches can often cause long rebuilds, due to all the files that
   have changed.  Wouldn't it be great if build artifacts were associated with
   branches, so that no rebuild is needed after switching?

4. Some build systems and tools store absolute paths, so, in this case, copying
   the old artifacts won't work. Some build systems rely on modification
   timestamps, so copying also won't work for that case.

5. It isn't very efficient in disk usage, since it needs to create an entire
   worktree.  It gets even worse when it comes to build artifacts, since there
   are copies of those too.

Given all these problems, it isn't surprising that many programmers just stick
to one working copy, and stop making changes whenever they kick off a long
running process:

[![XKCD 303: The #1 programmer excuse for legitimately slacking off: "My code's
compiling"](https://imgs.xkcd.com/comics/compiling.png)](https://xkcd.com/303/)

Instead of slacking off after kicking off a long running task, perhaps you can
be mizerly with your time and get in the zone!

[`git worktree`]: https://git-scm.com/docs/git-worktree

### Q: Isn't this a generally useful thing beyond git?

A: Yes.  I think it makes sense to make this tool as generic as possible.  I'm
thinking of having the name just be `mizer`, and having some git specific logic
in the case that it's applied to a git repository.  For now, though, the focus
is on the usecase with `git`.

## Proxy-auto-discovery for command line applications (pac4cli)

![CircleCI](https://img.shields.io/circleci/project/github/tkluck/pac4cli.svg)

### Introduction

On many corporate networks, applications need
[proxy-auto-discovery](https://en.wikipedia.org/wiki/Web_Proxy_Auto-Discovery_Protocol)
to know whether a certain URL is accessed either directly or through a web
proxy. Browsers can typically handle this, but many command line applications
(git, npm, apt, curl) rely on environment variable to hard-code a proxy
regardless of the destination URL.

This little daemon enables these applications for auto-discovery by:

- setting the `http_proxy` variable (and friends) to http://localhost:3128
- providing a simple proxy at that port that does proxy-auto-discovery and
  connects accordingly.

System dependencies:
- systemd
- NetworkManager

Python library dependencies from PyPI can be installed through:

    sudo pip3 install -r requirements.txt

### Installation instructions

#### Ubuntu

The latest builds are available through a PPA:

    sudo add-apt-repository ppa:tkluck/pac4cli
    sudo apt update
    sudo apt install pac4cli

You'll need to restart your shell for the new environment variables to take effect.

#### Archlinux

This package is available in AUR.

#### Other (Mac, other linuxes)

The dependencies can be installed through

    pip3 install -r requirements.txt

Then, use

    make install

**Just `python setup.py install` is not sufficient.** That will only install
the pac4cli module, but not the binary or the configuration files.

from setuptools import setup, find_packages
setup(
    name="pac4cli",
    version="0.2",
    packages=find_packages(),
)
import sys
print("------------------------------------------------------------------------", file=sys.stderr)
print("Your operation was successful for the pac4cli module.", file=sys.stderr)
print("You probably want to run `make install` to install the full application.", file=sys.stderr)
print("------------------------------------------------------------------------", file=sys.stderr)

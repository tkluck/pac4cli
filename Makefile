SHELL = /bin/bash

PYTHON ?= "$(shell which python3 )"
TESTPORT ?= 23128

prefix = /usr/local
bindir := $(prefix)/bin
libdir := $(prefix)/lib
pythonsitedir = "$(shell $(PYTHON) -c "from distutils.sysconfig import get_python_lib; print(get_python_lib())" )"

default:
	@echo Nothing to build\; run make install.

pacparser:
	curl -L https://github.com/pacparser/pacparser/archive/1.3.7.tar.gz | tar -xz
	mv pacparser-1.3.7 pacparser

env: requirements.txt pacparser
	virtualenv -p $(PYTHON) env
	env/bin/pip install -r requirements.txt
	PYTHON=`pwd`/env/bin/python make -C pacparser/src install-pymod

run: env
	env/bin/python main.py -F DIRECT -p $(TESTPORT)

check: env
	./testrun.sh $(TESTPORT)

check-prev-proxies:
	@RESULT=$$(grep -r --color -E '(http_proxy=)|(HTTP_PROXY=)|(https_proxy=)|(HTTPS_PROXY=)' $(DESTDIR)/etc/profile.d | cut -d' ' -f1 | sort | uniq) && \
	if [[ "x$$RESULT" != "x" ]];then \
		echo "Found these scripts setting the enviroment variables http_proxy & HTTP_PROXY:" && \
		while IFS=' ' read -ra FILES; do \
			for FILE in "$${FILES[@]}"; do \
				echo $${FILE::-1}; \
			done; \
		done  <<< "$$RESULT" && \
		echo "You have to either remove those definitions, or set them manually to 'localhost:3128'." && \
		echo "Otherwise, pac4cli may fail to work properly."; \
	fi

install-service: check-prev-proxies
	install -D -m 644 pac4cli.service $(DESTDIR)$(libdir)/systemd/system/pac4cli.service
	
	@sed -i -e 's@/usr/local/bin@'"$(bindir)"'@g' $(DESTDIR)$(libdir)/systemd/system/pac4cli.service

	install -D -m 755 trigger-pac4cli $(DESTDIR)/etc/NetworkManager/dispatcher.d/trigger-pac4cli
	install -D -m 755 pac4cli.sh $(DESTDIR)/etc/profile.d/pac4cli-proxy.sh

install-bin:
	install -D -m 755 main.py $(DESTDIR)$(bindir)/pac4cli
	@sed -i -e '1s+@PYTHON@+'$(PYTHON)'+' $(DESTDIR)$(bindir)/pac4cli

	install -D -m 644 pac4cli.py $(DESTDIR)$(pythonsitedir)/pac4cli.py

install: install-bin install-service

uninstall:
	$(shell $(DESTDIR)/uninstall.sh $(DESTDIR)/)

clean:
	rm -rf env
	rm -rf pacparser

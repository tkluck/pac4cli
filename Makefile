PYTHON ?= python3
TESTPORT ?= 23128
SHELL = /bin/bash

prefix = /usr/local
bindir := $(prefix)/bin
libdir := $(prefix)/lib
pythonsitedir = $(prefix)/lib/python3/site-packages

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

install-self-contained: check-prev-proxies pacparser
	virtualenv -p $(PYTHON) --system-site-packages $(DESTDIR)/opt/pac4cli
	$(DESTDIR)/opt/pac4cli/bin/pip install -r requirements.txt
	PYTHON=$(DESTDIR)/opt/pac4cli/bin/python make -C pacparser/src install-pymod

	install -m 755 -d $(DESTDIR)/opt/pac4cli
	install -m 644 main.py proxy.py $(DESTDIR)/opt/pac4cli
	install -m 755 uninstall.sh $(DESTDIR)/opt/pac4cli
	install -D -m 644 pac4cli.service $(DESTDIR)/lib/systemd/system/pac4cli.service
	install -D -m 755 trigger-pac4cli $(DESTDIR)/etc/NetworkManager/dispatcher.d/trigger-pac4cli
	install -D -m 755 pac4cli.sh $(DESTDIR)/etc/profile.d/pac4cli.sh


install-service:
	install -D -m 644 pac4cli.service $(DESTDIR)$(libdir)/systemd/system/pac4cli.service
	
	@sed -i -e 's@/usr/local/bin@'"$(bindir)"'@g' $(DESTDIR)$(libdir)/systemd/system/pac4cli.service

	install -D -m 755 trigger-pac4cli $(DESTDIR)/etc/NetworkManager/dispatcher.d/trigger-pac4cli
	install -D -m 755 pac4cli.sh $(DESTDIR)/etc/profile.d/pac4cli-proxy.sh

install-bin:
	install -D -m 755 main.py $(DESTDIR)$(bindir)/pac4cli
	install -D -m 644 proxy.py $(DESTDIR)$(pythonsitedir)/proxy.py

install: install-bin install-service

uninstall:
	$(shell $(DESTDIR)/uninstall.sh $(DESTDIR)/)
	rm -rf $(DESTDIR)/opt/pac4cli

clean:
	rm -rf env
	rm -rf pacparser

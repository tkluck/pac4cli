PYTHON ?= python3
PORT ?= 3128

SHELL = /bin/bash

PREFIX ?= /usr

default:
	@echo Nothing to build\; run make install.

env: requirements.txt
	virtualenv -p $(PYTHON) env
	env/bin/pip install -r requirements.txt
	PYTHON=`pwd`/env/bin/python make -C pacparser/src install-pymod

run:
	env/bin/python main.py -F DIRECT -p $(PORT)

check:
	./testrun.sh $(PORT)

check-prev-proxies:
	@RESULT=$$(grep -r --color -E '(http_proxy=)|(HTTP_PROXY=)|(https_proxy=)|(HTTPS_PROXY=)' $(DESTDIR)/$(PREFIX)/etc/profile.d | cut -d' ' -f1 | sort | uniq) && \
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

install-self-contained: check-prev-proxies
	virtualenv -p $(PYTHON) --system-site-packages $(DESTDIR)/opt/pac4cli
	$(DESTDIR)/opt/pac4cli/bin/pip install -r requirements.txt
	PYTHON=$(DESTDIR)/opt/pac4cli/bin/python make -C pacparser/src install-pymod

	install -m 755 -d $(DESTDIR)/opt/pac4cli
	install -m 644 main.py proxy.py $(DESTDIR)/opt/pac4cli
	install -m 755 uninstall.sh $(DESTDIR)/opt/pac4cli
	install -D -m 644 pac4cli.service $(DESTDIR)/lib/systemd/system/pac4cli.service
	install -D -m 755 trigger-pac4cli $(DESTDIR)/etc/NetworkManager/dispatcher.d/trigger-pac4cli
	install -D -m 755 pac4cli.sh $(DESTDIR)/etc/profile.d/pac4cli.sh

install: check-prev-proxies
	install -D -m 755 main.py $(DESTDIR)/$(PREFIX)/bin/pac4cli
	install -D -m 644 proxy.py $(DESTDIR)/$(PREFIX)/lib/python3.6/site-packages
	install -D -m 644 pac4cli.service $(DESTDIR)/$(PREFIX)/lib/systemd/system/pac4cli.service
	install -D -m 755 trigger-pac4cli $(DESTDIR)/$(PREFIX)/etc/NetworkManager/dispatcher.d/trigger-pac4cli
	install -D -m 755 pac4cli.sh $(DESTDIR)/$(PREFIX)/etc/profile.d/pac4cli-proxy.sh

install-debian: install

uninstall:
	$(shell $(DESTDIR)/uninstall.sh $(DESTDIR)/)
	rm -rf $(DESTDIR)/opt/pac4cli

clean:
	rm -rf env

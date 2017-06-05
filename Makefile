PYTHON ?= python3
PORT ?= 3128

SHELL = /bin/bash

DESTDIR = /

env: requirements.txt
	virtualenv -p $(PYTHON) env
	env/bin/pip install -r requirements.txt
	PYTHON=`pwd`/env/bin/python make -C pacparser/src install-pymod

run:
	env/bin/python main.py -F DIRECT -p $(PORT)

check:
	./testrun.sh $(PORT)

install:
	systemctl stop pac4cli.service || true
	virtualenv -p $(PYTHON) $(DESTDIR)opt/pac4cli
	$(DESTDIR)opt/pac4cli/bin/pip install -r requirements.txt
	PYTHON=$(DESTDIR)opt/pac4cli/bin/python make -C pacparser/src install-pymod
	install -m 644 main.py proxy.py uninstall.sh $(DESTDIR)opt/pac4cli

	install -D -m 644 pac4cli.service $(DESTDIR)lib/systemd/system/pac4cli.service

ifeq ( $(DESTDIR), / )
	install -D -m 755 -o root -g root trigger-pac4cli $(DESTDIR)etc/NetworkManager/dispatcher.d/trigger-pac4cli
else
	install -D -m 755 trigger-pac4cli $(DESTDIR)etc/NetworkManager/dispatcher.d/trigger-pac4cli
endif

	@RESULT=$$(grep -r --color -E '(\$$http_proxy)|(\$$HTTP_PROXY)' $(DESTDIR)etc/profile.d | cut -d' ' -f1 | sort | uniq) && \
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

	install -D -m 755 pac4cli.sh $(DESTDIR)etc/profile.d

uninstall:
	$(shell $(DESTDIR)uninstall.sh $(DESTDIR))
	rm -rf $(DESTDIR)opt/pac4cli

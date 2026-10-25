#!/bin/sh
# Fake email CLI for parcel's tests: logs its argv to $1, prints canned output.
log=$1; shift
echo "$*" >> "$log"
case "$1" in
  mailboxes) echo '{"boxes":[{"name":"Inbox"},{"name":"Archive"}]}' ;;
  list) echo '{"items":[
    {"id":"1","subject":"Hello","from":{"name":"Ada"},"date":"2026-10-25T10:00:00Z","flags":["seen"]},
    {"id":"2","subject":"Bye","from":{"name":null,"email":"bob@x.io"},"date":"2026-10-24T10:00:00Z","flags":[{"name":"Flagged"}]}]}' ;;
  read) printf 'From: Ada\nSubject: Hello\n\nHi there\n' ;;
  reply) printf 'Subject: Re: Hello\n\n> Hi there\n' ;;
  send) cat >> "$log" ;;
  fail) echo "boom: mailbox gone" >&2; exit 3 ;;
esac

Feature: Kanari CLI rename module friend list

    @serial
    Scenario: friend_module_renaming
        Given a server for friend_module_renaming
        Then cmd: "account list --json"

        Then cmd: "move publish -p ../../examples/rename_friend_module  --named-addresses kanari_examples=default --json"
        Then assert: "{{$.move[-1].execution_info.status.type}} == executed"

        Then cmd: "move publish -p ../../examples/rename_friend_module_trigger  --named-addresses kanari_examples=default --skip-client-compat-check --json"
        Then assert: "{{$.move[-1].execution_info.status.type}} == moveabort"

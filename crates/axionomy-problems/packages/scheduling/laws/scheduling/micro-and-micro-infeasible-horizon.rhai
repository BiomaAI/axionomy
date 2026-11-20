[
    [
        "finish/{job-one-end}/{job-two-end}/{makespan}",
        #{
            accounts: [["primary-job", "job/one"], ["schedule", "success"], ["secondary-job", "job/two"]],
            consume: [["schedule", [["active", "1"]]]],
            distinct: [],
            preserve: [
                ["primary-job", [["completed-at/one-b/{job-one-end}", "1"]]],
                ["secondary-job", [["completed-at/two-b/{job-two-end}", "1"]]]
            ],
            produce: [["schedule", [["makespan/{makespan}", "1"], ["solved", "1"]]]]
        }
    ]
]

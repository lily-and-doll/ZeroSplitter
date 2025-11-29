SELECT
split_num,
count(case when final = false then 1 end) as pass_count,
count(case when final = false or final=true then 1 end) as run_count,
count(case when final = false then 1 end) * 100.0 / count(case when final = false or final=true then 1 end) as percentage
FROM splits INNER JOIN runs ON runs.id = splits.run_id
WHERE score > 0 and runs.category = 3
GROUP BY split_num